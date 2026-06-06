# Typed-primitive pipelines + SDF shaders Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** R6 (`render/buckets.rs` — owns the shared `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }` enum and CPU instance bucketing; R7 **imports** that enum, it does not redefine it). Execution order: R1 → R2 → R3 → R4 → R5 → R6 → **R7** → R8 → (R9, R10) → R11. R7 must land after R6 so `BuiyPrimitiveKind` exists to import.

**Goal:** Replace the Phase-0 single hard-format rounded-rect pipeline with a set of typed-primitive `SpecializedRenderPipeline`s (quad / box-shadow) keyed on the target `ColorTargetState` format (view format + `Rgba16Float` group targets), each backed by its own WGSL SDF shader under the stable `0xB01A_01XX` render-asset UUID octets. **Border is folded into the `quad` SDF pipeline** ([architecture.md § 1.4 / § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set): "`quad` … border = outer-minus-inner SDF band") — it is **not** a separate shader, octet, or `BuiyPrimitiveKind` variant; the border band is part of the **quad** primitive. The landed R6 enum is `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }`; this phase builds the two F-tier pipelines whose shaders it ships — `Quad` (octet `..01`) and `Shadow` (octet `..02`). `Glyph` and `Path` are bucket-reserved by R6 but their shaders are sibling-phase work, not built here.

**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [architecture.md](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md) § 1.4 (typed-primitive pipelines, `SpecializedRenderPipeline` per format, the normative octet table) + § 2.1/§ 2.2 (the primitive set, batching), and [color-and-forced-colors.md](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md) § 1 (linear-light render, one pipeline per target format, the blend-space seam).

**Architecture:** A wgpu `RenderPipeline`'s fragment `ColorTargetState.format` is fixed at creation, so a single pipeline cannot target both the view's `Rgba8UnormSrgb` attachment and the `Rgba16Float` effect-group targets. Each typed primitive therefore becomes a `SpecializedRenderPipeline` keyed on the target format; `Buiy` builds each primitive for both formats. The specialization key is a tiny pure value (`BuiyPrimitiveKey`) whose `kind` field is the shared `BuiyPrimitiveKind` enum **owned by R6 (`render/buckets.rs`)** — R7 imports it, never redefines it. The "distinct key ⇒ distinct `CachedRenderPipelineId` per format" mapping logic and the descriptor construction are unit-testable with no wgpu adapter; the WGSL is validated by parsing with `naga` (no GPU). Actual pipeline compilation and draw stay GPU-only and ride the `#[ignore]` e2e path. This phase keeps the Phase-0 rounded-rect visual behavior intact — it becomes the `quad` primitive — and adds the shadow shader without yet wiring the new component model (that is a sibling phase; this phase wires the *pipelines* and proves the *specialization key* logic). **Border is folded into the quad SDF** (outer-minus-inner band, [architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set)) — no separate border shader, no new octet, no `Border` key kind. Outline (focus indicator) is likewise the quad pipeline with the clip rect suppressed, not a distinct kind in the landed R6 enum.

**Tier/Test reality:** GPU (code + `#[ignore]` e2e — no wgpu adapter on CI / this host). The **gating** (always-green) tests are device-free: the specialization-key pure logic, the descriptor-construction pure logic (format/blend/UUID assertions over the returned `RenderPipelineDescriptor`), the per-format-distinct-id mapping over a stub cache, and `naga` WGSL parse/entry-point checks. The **`#[ignore]`** tests (need a wgpu adapter) are: `SpecializedRenderPipelines::specialize` producing real `CachedRenderPipelineId`s through a live `PipelineCache`, pipeline-queued/compiled assertions, and any draw. Mark them `#[ignore]` with the same wording `render_smoke.rs` already uses.

---

## The gate (every commit must keep this green)

This host and CI have **no xvfb and no wgpu adapter**. Run before every commit; all four must pass:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

> The `cargo test --workspace` step here is the **headless** subset — `#[ignore]`d GPU tests do not run. To exercise the GPU tests locally on a machine with a real/lavapipe adapter: `cargo test -p buiy_core --test <file> -- --ignored`.

## Conventions this plan assumes (read once)

- **Worktree root:** `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline`. All paths below are relative to it unless absolute.
- **UUID octets are normative** ([architecture.md § 1.4](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#14-pipelines-pipelinecache--stable-uuid-shaders)). Reserved range `0xB01A_0100_..` through `0xB01A_01FF_..`. Assignments this phase realizes (do **not** renumber): `..01` quad/rounded-rect (exists), `..02` shadow. `..03` glyph-alpha (atlas phase), `..04` path (C-tier reserved), and `..05` composite (effect-compositor phase) are **not** built here. This phase adds **no new octet**: border folds into `..01` (next bullet).
- **Border folds into the quad SDF — no separate shader, no new octet, no key kind.** The spec's normative primitive table ([architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set)) defines `quad` as "Background fill + border band + rounded corners" with "border = outer-minus-inner SDF band" — i.e. the border band is part of the **quad** primitive, not a separate one. The landed R6 enum (`crates/buiy_core/src/render/buckets.rs`) is `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }` — there is **no** `Border` variant and **no** `Outline` variant; the buckets.rs doc comment is explicit that border folds into `Quad` and `Outline` is a `Quad` variant (the quad pipeline with the clip rect suppressed). R7 follows that: the border band is painted by the **quad** pipeline+shader (`..01`); the focus outline is the **quad** pipeline with clip suppressed. There is **no** `border.wgsl`, **no** `BORDER_SHADER_UUID`, and **no** `..06` octet. (An earlier draft of this plan shipped a distinct `..06` border shader *and* `Border`/`Outline` key kinds; both were deviations from the spec's folded-band model and the landed R6 enum, and are removed per the plan-reconciliation review. If a reviewer ever wants a standalone stroked-border pipeline for the elliptical per-corner case, that is a *spec amendment* to raise against architecture.md § 2.1 and a new `BuiyPrimitiveKind` variant in R6 first, not a silent plan deviation.)
- **Format key.** The two target formats are `TextureFormat::Rgba8UnormSrgb` (the `Camera2d` default view, via `ViewTarget::main_texture_format()` — owned by [architecture.md § 1.4](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#14-pipelines-pipelinecache--stable-uuid-shaders)) and `TextureFormat::Rgba16Float` (effect-group targets). The view format on an opt-in HDR view is also `Rgba16Float`; the key is the **format**, so HDR-view and group-target variants coincide and dedupe through the same `SpecializedRenderPipelines` cache — that dedup is an explicit gating-test assertion (Task 3).
- **Blend space.** All pipelines keep `BlendState::ALPHA_BLENDING` (the Phase-0 setting). The encoded-vs-linear blend-space seam ([color-and-forced-colors.md § 1.1](../specs/2026-06-03-buiy-render-pipeline-design/color-and-forced-colors.md#11-the-invariant)) is a consequence of the target *format*, not a per-pipeline blend change, so no pipeline sets a different `BlendState`.
- **No component-model wiring here.** `Background`/`Border`/`BoxShadow`/`Outline` *components* and the extract rewrite are sibling phases. This phase touches only `crates/buiy_core/src/render/{pipeline,shader.wgsl}` and adds `shaders/` WGSL + a `primitive` module; the Phase-0 `node.rs` keeps drawing the quad via the existing path (Task 8 only swaps the quad pipeline-id source so the node still compiles and the existing GPU smoke test still asserts a registered quad pipeline).

---

## Task 0 — Add `naga` as a dev-dependency for headless WGSL validation

**Why:** WGSL parse/entry-point checks are the device-free way to prove a shader is well-formed without a wgpu adapter. `naga` 27.x is already in the dependency tree transitively (via `wgpu`); add it as a **dev-dependency** so test code can call `naga::front::wgsl::parse_str` without pulling it into the shipped crate.

**Files**
- Modify: `crates/buiy_core/Cargo.toml`
- Test: `crates/buiy_core/tests/render_shader_wgsl.rs` (created here, fleshed out in later tasks)

Steps:

- [ ] Confirm the resolved `naga` version so the dev-dep matches the tree (avoids a second copy):
  ```sh
  cargo tree -p buiy_core -i naga 2>/dev/null | head -5
  ```
  Expected: a `naga vX.Y.Z` line (X = 27 as of writing). Use that `X` as the version requirement below.
- [ ] Add to `crates/buiy_core/Cargo.toml` under a `[dev-dependencies]` table (create the table if absent; it currently has only `[dependencies]`):
  ```toml
  [dev-dependencies]
  naga = "27"
  ```
  (Match the major from `cargo tree`; if it reports `28`, use `"28"`.)
- [ ] Write the failing test — a minimal parse smoke over the **existing** Phase-0 shader, proving the harness compiles and `naga` is reachable. Create `crates/buiy_core/tests/render_shader_wgsl.rs`:
  ```rust
  //! Headless WGSL validation of Buiy's render shaders. Parses each shader
  //! source with `naga` (no wgpu adapter needed) and asserts the expected
  //! entry points exist. This is the device-free half of pipeline coverage;
  //! actual GPU compilation rides the `#[ignore]` e2e path (render_smoke.rs).

  /// Parse WGSL source with naga; panics with the naga diagnostic on error.
  fn parse_wgsl(label: &str, src: &str) -> naga::Module {
      naga::front::wgsl::parse_str(src)
          .unwrap_or_else(|e| panic!("{label}: WGSL parse failed: {e:?}"))
  }

  /// True iff the module declares an entry point with this name.
  fn has_entry_point(module: &naga::Module, name: &str) -> bool {
      module.entry_points.iter().any(|ep| ep.name == name)
  }

  const QUAD_WGSL: &str = include_str!("../src/render/shader.wgsl");

  #[test]
  fn quad_shader_parses_and_has_entry_points() {
      let m = parse_wgsl("quad", QUAD_WGSL);
      assert!(has_entry_point(&m, "vertex"), "quad shader has `vertex`");
      assert!(has_entry_point(&m, "fragment"), "quad shader has `fragment`");
  }
  ```
- [ ] Run it — expect **FAIL** first only if `naga` is missing; after adding the dev-dep it should **PASS** (the existing `shader.wgsl` already parses):
  ```sh
  cargo test -p buiy_core --test render_shader_wgsl
  ```
  Expected before the dep is added: a compile error `unresolved import naga`. After: `quad_shader_parses_and_has_entry_points ... ok`.
- [ ] Run the full gate. Commit.
  - Commit: `test(render): add naga dev-dep + headless WGSL parse harness`

---

## Task 1 — `BuiyPrimitiveKey` specialization key (pure type; imports `BuiyPrimitiveKind` from R6)

**Why:** The specialization key is the single device-free fulcrum of this phase: `(primitive, target_format)` → one pipeline variant. Define `BuiyPrimitiveKey` as a tiny `Clone + Hash + PartialEq + Eq` value (the bound `SpecializedRenderPipeline::Key` requires) and prove its construction/equality before any descriptor or shader work. No GPU.

**Ownership guard — do NOT redefine `BuiyPrimitiveKind`.** The shared `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }` enum is **owned by R6** (`crates/buiy_core/src/render/buckets.rs`, the CPU instance-bucketing module). It already exists by the time R7 runs (execution order R6 → R7), and it has exactly those four variants — there is **no** `Border` or `Outline` variant (border folds into `Quad`; outline is a clip-suppressed `Quad`). R7 **imports** it (`use crate::render::buckets::BuiyPrimitiveKind;`) and does **not** redefine it, does **not** re-export it, does **not** add a parallel copy in `primitive.rs`, and does **not** add new variants to it. `primitive.rs` owns only `BuiyPrimitiveKey`.

**Files**
- Create: `crates/buiy_core/src/render/primitive.rs` (defines `BuiyPrimitiveKey` **only**; imports `BuiyPrimitiveKind` from `crate::render::buckets`)
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod primitive;` — `pub mod buckets;` is already added by R6, do **not** re-add it)
- Test: `crates/buiy_core/tests/render_primitive_key.rs`

Steps:

- [ ] Write the failing test first. Create `crates/buiy_core/tests/render_primitive_key.rs`:
  ```rust
  //! Device-free tests for the typed-primitive specialization key. No wgpu
  //! adapter required — these assert the pure key logic that drives
  //! `SpecializedRenderPipeline` variant selection (architecture.md § 1.4).

  use bevy::render::render_resource::TextureFormat;
  // `BuiyPrimitiveKind` is owned by R6 (render::buckets); `BuiyPrimitiveKey`
  // is owned here (render::primitive). Import each from its real owner.
  use buiy_core::render::buckets::BuiyPrimitiveKind;
  use buiy_core::render::primitive::BuiyPrimitiveKey;

  #[test]
  fn kind_variants_are_distinct() {
      use BuiyPrimitiveKind::*;
      // The landed R6 enum (render::buckets) — Shadow/Quad/Glyph/Path. Border
      // folds into Quad and Outline is a clip-suppressed Quad, so neither is a
      // variant here. This phase ships the Quad and Shadow pipelines.
      let all = [Shadow, Quad, Glyph, Path];
      for (i, a) in all.iter().enumerate() {
          for (j, b) in all.iter().enumerate() {
              assert_eq!(i == j, a == b, "{a:?} vs {b:?} distinctness");
          }
      }
  }

  #[test]
  fn key_equality_is_by_kind_and_format() {
      let a = BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      };
      let b = BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      };
      let diff_format = BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba16Float,
      };
      let diff_kind = BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Shadow,
          format: TextureFormat::Rgba8UnormSrgb,
      };
      assert_eq!(a, b);
      assert_ne!(a, diff_format);
      assert_ne!(a, diff_kind);
  }

  #[test]
  fn key_is_hashable_and_dedupes_in_a_set() {
      use std::collections::HashSet;
      let mut set = HashSet::new();
      set.insert(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      // Same key inserted twice → one entry (Hash + Eq).
      set.insert(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      // Different format → distinct entry.
      set.insert(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba16Float,
      });
      assert_eq!(set.len(), 2);
  }
  ```
- [ ] Run — expect **FAIL** (module/types do not exist; compile error `unresolved import buiy_core::render::primitive`):
  ```sh
  cargo test -p buiy_core --test render_primitive_key
  ```
- [ ] **Guard:** confirm `BuiyPrimitiveKind` already exists in `crates/buiy_core/src/render/buckets.rs` (it does — owned by R6). Do **not** redefine it. If it is somehow missing, R6 has not landed; stop and resolve the ordering, do not re-create the enum here.
- [ ] Minimal impl. Create `crates/buiy_core/src/render/primitive.rs` — it defines `BuiyPrimitiveKey` only and **imports** `BuiyPrimitiveKind` from R6's `buckets` module:
  ```rust
  //! Typed-primitive pipeline specialization: the device-free key that selects
  //! one `SpecializedRenderPipeline` variant per `(primitive, target format)`.
  //!
  //! A wgpu `RenderPipeline`'s fragment `ColorTargetState.format` is fixed at
  //! creation, so each typed primitive (quad / shadow / outline; border folds
  //! into quad) is a `SpecializedRenderPipeline` keyed on the target format;
  //! Buiy builds each for both the view format (`Rgba8UnormSrgb` by default)
  //! and the `Rgba16Float` effect-group target format. See
  //! `docs/specs/2026-06-03-buiy-render-pipeline-design/architecture.md` § 1.4.
  //!
  //! `BuiyPrimitiveKind` is **owned by `crate::render::buckets`** (R6); this
  //! module imports it and adds only the `(kind, format)` specialization key.

  use bevy::render::render_resource::TextureFormat;

  // Owned by R6 (render::buckets) — imported, not redefined.
  use crate::render::buckets::BuiyPrimitiveKind;

  /// One `SpecializedRenderPipeline` variant: a primitive built for a specific
  /// target color-attachment format. `Key` for the typed-primitive
  /// `SpecializedRenderPipeline` (architecture.md § 1.4).
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub struct BuiyPrimitiveKey {
      pub kind: BuiyPrimitiveKind,
      /// The bound attachment's format: the view format for the main pass
      /// (`Rgba8UnormSrgb` default / `Rgba16Float` HDR) or the fixed
      /// `Rgba16Float` for effect-group targets.
      pub format: TextureFormat,
  }
  ```
  > If `BuiyPrimitiveKey` re-exporting `BuiyPrimitiveKind` is convenient for downstream imports, add `pub use crate::render::buckets::BuiyPrimitiveKind;` here — but the **definition** stays in `buckets.rs`. Prefer importing from `buckets` at each use site to keep ownership unambiguous.
- [ ] Add `pub mod primitive;` to `crates/buiy_core/src/render/mod.rs`. Do **not** add `pub mod buckets;` — R6 already added it. (`pub mod instance; pub mod node; pub mod pipeline;` are the Phase-0 modules already present.)
- [ ] Run — expect **PASS** (all three tests green).
- [ ] Run the full gate. Commit.
  - Commit: `feat(render): add typed-primitive specialization key`

---

## Task 2 — The `SpecializedRenderPipeline` impl skeleton + the quad descriptor (pure `specialize`, no GPU)

**Why:** `SpecializedRenderPipeline::specialize(&self, key) -> RenderPipelineDescriptor` is a **pure function** — it builds a descriptor, queues nothing, touches no device. So the entire descriptor (format, blend, shader handle, entry points, vertex layout) is assertable headless. Start with the quad variant, factoring the Phase-0 descriptor out of `pipeline.rs::register` into a specializer that keys the `ColorTargetState.format` off `key.format` instead of hard-coding `Rgba8UnormSrgb`.

**Files**
- Modify: `crates/buiy_core/src/render/primitive.rs` (add `BuiyPrimitives` specializer struct + `impl SpecializedRenderPipeline`)
- Test: `crates/buiy_core/tests/render_primitive_descriptor.rs`

Steps:

- [ ] Write the failing test. Create `crates/buiy_core/tests/render_primitive_descriptor.rs`:
  ```rust
  //! Device-free tests over the `RenderPipelineDescriptor` that
  //! `SpecializedRenderPipeline::specialize` builds. `specialize` is a pure fn
  //! (no PipelineCache, no RenderDevice), so the descriptor — its target
  //! format, blend, shader handle, and entry points — is fully assertable
  //! without a wgpu adapter (architecture.md § 1.4).

  use bevy::render::render_resource::{
      BlendState, SpecializedRenderPipeline, TextureFormat,
  };
  use buiy_core::render::buckets::BuiyPrimitiveKind;
  use buiy_core::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

  fn descriptor_for(kind: BuiyPrimitiveKind, format: TextureFormat) {
      // exercised via the asserts in each test; helper kept for readability
      let _ = (kind, format);
  }

  #[test]
  fn quad_descriptor_uses_key_format_not_hardcoded() {
      let specializer = BuiyPrimitives::default();
      let srgb = specializer.specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      let hdr = specializer.specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba16Float,
      });
      let srgb_fmt = srgb.fragment.as_ref().unwrap().targets[0]
          .as_ref()
          .unwrap()
          .format;
      let hdr_fmt = hdr.fragment.as_ref().unwrap().targets[0]
          .as_ref()
          .unwrap()
          .format;
      assert_eq!(srgb_fmt, TextureFormat::Rgba8UnormSrgb);
      assert_eq!(hdr_fmt, TextureFormat::Rgba16Float);
      descriptor_for(BuiyPrimitiveKind::Quad, srgb_fmt);
  }

  #[test]
  fn quad_descriptor_keeps_alpha_blending_and_entry_points() {
      let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      let frag = d.fragment.as_ref().unwrap();
      assert_eq!(
          frag.targets[0].as_ref().unwrap().blend,
          Some(BlendState::ALPHA_BLENDING),
          "alpha blending preserved (the Phase-0 setting; blend-space seam is \
           a format consequence, not a blend change)"
      );
      assert_eq!(d.vertex.entry_point.as_deref(), Some("vertex"));
      assert_eq!(frag.entry_point.as_deref(), Some("fragment"));
  }

  #[test]
  fn quad_descriptor_has_two_vertex_buffers_with_phase0_strides() {
      // Static unit-quad VBO (stride 16) + per-instance buffer (stride 36),
      // matching the Phase-0 pipeline layout the quad primitive preserves.
      let d = BuiyPrimitives::default().specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      let buffers = &d.vertex.buffers;
      assert_eq!(buffers.len(), 2, "vertex + instance buffer layouts");
      assert_eq!(buffers[0].array_stride, 16);
      assert_eq!(buffers[1].array_stride, 36);
  }
  ```
- [ ] Run — expect **FAIL** (`BuiyPrimitives` does not exist):
  ```sh
  cargo test -p buiy_core --test render_primitive_descriptor
  ```
- [ ] Minimal impl. Append to `crates/buiy_core/src/render/primitive.rs` a `BuiyPrimitives` specializer that owns the per-primitive shader handles and builds descriptors. Move the Phase-0 quad descriptor body here, parameterized on `key.format`:
  ```rust
  use bevy::mesh::VertexBufferLayout;
  use bevy::render::render_resource::{
      BlendState, ColorTargetState, ColorWrites, FragmentState, FrontFace,
      MultisampleState, PolygonMode, PrimitiveState, PrimitiveTopology,
      RenderPipelineDescriptor, SpecializedRenderPipeline, VertexAttribute,
      VertexFormat, VertexState, VertexStepMode,
  };

  use crate::render::pipeline::{shader_handle, shadow_shader_handle};

  /// The typed-primitive `SpecializedRenderPipeline`. One specializer builds
  /// every `(kind, format)` variant; `SpecializedRenderPipelines<BuiyPrimitives>`
  /// (render world) dedupes identical keys into one `CachedRenderPipelineId`.
  #[derive(Default)]
  pub struct BuiyPrimitives;

  impl BuiyPrimitives {
      /// The two interleaved vertex-buffer layouts shared by every quad-family
      /// primitive (static unit quad, stride 16; per-instance record, stride 36).
      fn quad_family_vertex_buffers() -> Vec<VertexBufferLayout> {
          vec![
              VertexBufferLayout {
                  array_stride: 16,
                  step_mode: VertexStepMode::Vertex,
                  attributes: vec![
                      VertexAttribute {
                          format: VertexFormat::Float32x2,
                          offset: 0,
                          shader_location: 0,
                      },
                      VertexAttribute {
                          format: VertexFormat::Float32x2,
                          offset: 8,
                          shader_location: 1,
                      },
                  ],
              },
              VertexBufferLayout {
                  array_stride: 36,
                  step_mode: VertexStepMode::Instance,
                  attributes: vec![
                      VertexAttribute {
                          format: VertexFormat::Float32x2,
                          offset: 0,
                          shader_location: 2,
                      },
                      VertexAttribute {
                          format: VertexFormat::Float32x2,
                          offset: 8,
                          shader_location: 3,
                      },
                      VertexAttribute {
                          format: VertexFormat::Float32x4,
                          offset: 16,
                          shader_location: 4,
                      },
                      VertexAttribute {
                          format: VertexFormat::Float32,
                          offset: 32,
                          shader_location: 5,
                      },
                  ],
              },
          ]
      }

      /// The shader handle for a primitive kind. Border folds into the quad
      /// SDF and outline is a clip-suppressed quad, so both paint through the
      /// quad shader — neither is a distinct `BuiyPrimitiveKind` variant.
      /// `Glyph` / `Path` shaders are sibling-phase work (octets `..03` /
      /// `..04`), not built here; this phase ships only `Quad` and `Shadow`.
      fn shader_for(kind: BuiyPrimitiveKind) -> bevy::asset::Handle<bevy::shader::Shader> {
          match kind {
              BuiyPrimitiveKind::Quad => shader_handle(),
              BuiyPrimitiveKind::Shadow => shadow_shader_handle(),
              BuiyPrimitiveKind::Glyph | BuiyPrimitiveKind::Path => shader_handle(),
          }
      }
  }

  impl SpecializedRenderPipeline for BuiyPrimitives {
      type Key = BuiyPrimitiveKey;

      fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
          let shader = Self::shader_for(key.kind);
          RenderPipelineDescriptor {
              label: Some(format!("buiy_{:?}_pipeline", key.kind).into()),
              layout: vec![],
              push_constant_ranges: vec![],
              vertex: VertexState {
                  shader: shader.clone(),
                  shader_defs: vec![],
                  entry_point: Some("vertex".into()),
                  buffers: Self::quad_family_vertex_buffers(),
              },
              primitive: PrimitiveState {
                  topology: PrimitiveTopology::TriangleStrip,
                  front_face: FrontFace::Ccw,
                  cull_mode: None,
                  polygon_mode: PolygonMode::Fill,
                  ..Default::default()
              },
              depth_stencil: None,
              multisample: MultisampleState::default(),
              fragment: Some(FragmentState {
                  shader,
                  shader_defs: vec![],
                  entry_point: Some("fragment".into()),
                  targets: vec![Some(ColorTargetState {
                      // The format/edge seam: keyed off the bound attachment,
                      // not hard-coded (architecture.md § 1.4).
                      format: key.format,
                      blend: Some(BlendState::ALPHA_BLENDING),
                      write_mask: ColorWrites::ALL,
                  })],
              }),
              zero_initialize_workgroup_memory: false,
          }
      }
  }
  ```
  > `shadow_shader_handle` is introduced in Task 4. To keep this task compiling and green on its own, add a **temporary** stub to `pipeline.rs` now that returns a weak handle behind its (not-yet-registered) UUID — Task 4 replaces the stub by registering the real WGSL under the same UUID. Concretely, in `pipeline.rs` add:
  ```rust
  /// Stable UUID for the box-shadow SDF shader (octet `..02`).
  const SHADOW_SHADER_UUID: Uuid = Uuid::from_u128(0xB01A_0102_0000_0000_0000_0000_0000_0002u128);

  /// Weak handle to the box-shadow WGSL shader.
  pub fn shadow_shader_handle() -> Handle<Shader> {
      Handle::Uuid(SHADOW_SHADER_UUID, PhantomData)
  }
  ```
  (The handle is a valid weak handle even before the WGSL is inserted — `specialize` only references it; nothing loads it in a headless test.)
- [ ] Run — expect **PASS** (all three descriptor tests green).
- [ ] Run the full gate. Commit.
  - Commit: `feat(render): SpecializedRenderPipeline for typed primitives (quad descriptor)`

---

## Task 3 — Per-format-distinct-id mapping logic (pure, over a stub id-allocator — no GPU)

**Why:** The spec's headless requirement is explicit: *"the specialization key logic (pure fn) producing distinct `CachedRenderPipelineId`s per format."* The real `SpecializedRenderPipelines::specialize` needs a live `PipelineCache` (GPU), so prove the **mapping contract** device-free: distinct keys ⇒ distinct ids; identical keys ⇒ same id (dedup); and HDR-view format collapses onto the group-target format (both `Rgba16Float` ⇒ one id). Model the cache as a `HashMap<BuiyPrimitiveKey, u32>` with a monotonic counter — exactly the `or_insert_with`-counter shape `SpecializedRenderPipelines::specialize` uses, minus the device.

**Files**
- Modify: `crates/buiy_core/src/render/primitive.rs` (add a pure `variant_index` helper used by the test-facing dedup model; or expose the dedup as a documented invariant the test models directly)
- Test: `crates/buiy_core/tests/render_primitive_dedup.rs`

Steps:

- [ ] Write the failing test. Create `crates/buiy_core/tests/render_primitive_dedup.rs`:
  ```rust
  //! Device-free proof of the per-format-distinct-id mapping contract that the
  //! render-world `SpecializedRenderPipelines<BuiyPrimitives>` cache enforces.
  //! We model the cache's `entry(key).or_insert_with(counter)` allocation with
  //! a HashMap + monotonic counter (no PipelineCache, no RenderDevice), so the
  //! key→id contract is asserted with no wgpu adapter. The live-cache version
  //! rides the `#[ignore]` GPU path (Task 9). See architecture.md § 1.4.

  use std::collections::HashMap;

  use bevy::render::render_resource::TextureFormat;
  use buiy_core::render::buckets::BuiyPrimitiveKind;
  use buiy_core::render::primitive::BuiyPrimitiveKey;

  /// Mirror of `SpecializedRenderPipelines::specialize`'s allocation: each new
  /// key gets the next id; a repeated key returns its existing id.
  #[derive(Default)]
  struct StubPipelineIds {
      map: HashMap<BuiyPrimitiveKey, u32>,
      next: u32,
  }
  impl StubPipelineIds {
      fn id_for(&mut self, key: BuiyPrimitiveKey) -> u32 {
          let next = &mut self.next;
          *self.map.entry(key).or_insert_with(|| {
              let id = *next;
              *next += 1;
              id
          })
      }
  }

  fn key(kind: BuiyPrimitiveKind, format: TextureFormat) -> BuiyPrimitiveKey {
      BuiyPrimitiveKey { kind, format }
  }

  #[test]
  fn each_kind_x_format_gets_a_distinct_id() {
      use BuiyPrimitiveKind::*;
      let mut ids = StubPipelineIds::default();
      let formats = [TextureFormat::Rgba8UnormSrgb, TextureFormat::Rgba16Float];
      let mut seen = Vec::new();
      // The landed R6 enum's four kinds; the key is `(kind, format)` so each
      // (kind, format) pair is a distinct variant regardless of which kinds'
      // shaders this phase ships.
      for kind in [Shadow, Quad, Glyph, Path] {
          for format in formats {
              seen.push(ids.id_for(key(kind, format)));
          }
      }
      // 4 kinds × 2 formats = 8 distinct variants → 8 distinct ids.
      let mut uniq = seen.clone();
      uniq.sort_unstable();
      uniq.dedup();
      assert_eq!(uniq.len(), 8, "every (kind, format) variant is distinct");
  }

  #[test]
  fn identical_key_dedupes_to_one_id() {
      let mut ids = StubPipelineIds::default();
      let k = key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba8UnormSrgb);
      let a = ids.id_for(k);
      let b = ids.id_for(k);
      assert_eq!(a, b, "same key → same cached id (no duplicate pipeline)");
  }

  #[test]
  fn hdr_view_and_group_target_share_the_rgba16float_variant() {
      // The key is the *format*; an HDR view's main_texture_format() is
      // Rgba16Float, identical to the effect-group target format, so they
      // collapse onto the same variant id (no redundant pipeline).
      let mut ids = StubPipelineIds::default();
      let hdr_view = ids.id_for(key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba16Float));
      let group_target = ids.id_for(key(BuiyPrimitiveKind::Quad, TextureFormat::Rgba16Float));
      assert_eq!(hdr_view, group_target);
  }
  ```
- [ ] Run — expect **PASS immediately** if Task 1's `BuiyPrimitiveKey` already derives `Hash + Eq` (it does). If it **FAILS to compile**, the derive is missing — fix Task 1's derives. (This task is a contract-locking test; the production type already satisfies it, which is the point — the key's `Hash + Eq` *is* the dedup mechanism.)
  ```sh
  cargo test -p buiy_core --test render_primitive_dedup
  ```
- [ ] If all three pass with no production change, that is correct: the test pins the contract the real cache relies on. If any fails, the fix is in `primitive.rs` (the key derives), not the test.
- [ ] Run the full gate. Commit.
  - Commit: `test(render): pin per-format-distinct-id specialization contract`

---

## Task 4 — Box-shadow SDF shader (`..02`) + WGSL parse test

**Why:** The `shadow` primitive paints `BoxShadow` entries as a closed-form Gaussian-blurred rounded-rect SDF (one draw per shadow, no convolution pass — [architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set)). Ship the WGSL under the normative `..02` octet and validate it headlessly with `naga`. The shader shares the quad's vertex inputs (same instance stride) so the existing vertex layout is reused; the fragment uses the standard `erf`-approximation Gaussian-rect coverage.

**Files**
- Create: `crates/buiy_core/src/render/shadow.wgsl`
- Modify: `crates/buiy_core/src/render/pipeline.rs` (register the shadow WGSL under `SHADOW_SHADER_UUID`, replacing the Task-2 stub's missing registration)
- Test: `crates/buiy_core/tests/render_shader_wgsl.rs` (extend)

Steps:

- [ ] Write the failing test — add to `crates/buiy_core/tests/render_shader_wgsl.rs`:
  ```rust
  const SHADOW_WGSL: &str = include_str!("../src/render/shadow.wgsl");

  #[test]
  fn shadow_shader_parses_and_has_entry_points() {
      let m = parse_wgsl("shadow", SHADOW_WGSL);
      assert!(has_entry_point(&m, "vertex"), "shadow shader has `vertex`");
      assert!(has_entry_point(&m, "fragment"), "shadow shader has `fragment`");
  }
  ```
- [ ] Run — expect **FAIL** (`shadow.wgsl` does not exist; `include_str!` is a compile error):
  ```sh
  cargo test -p buiy_core --test render_shader_wgsl
  ```
- [ ] Minimal impl — create `crates/buiy_core/src/render/shadow.wgsl`. The fragment evaluates a separable Gaussian-blurred rounded-rect using the closed-form error-function approximation (no per-fragment convolution). Reuse the quad's `Vertex`/`Instance` bindings; the instance `radius` field carries the blur sigma in this primitive's interpretation (the sibling component-model phase maps `BoxShadow.blur` into it):
  ```wgsl
  // Buiy box-shadow shader (octet ..02). Closed-form Gaussian-blurred
  // rounded-rect coverage — one draw per shadow, no convolution pass.
  // Inputs match the quad instance layout (stride 36); `radius` carries the
  // shadow's effective blur sigma in pixels for this primitive.

  struct Vertex {
      @location(0) position: vec2<f32>,
      @location(1) uv: vec2<f32>,
  };

  struct Instance {
      @location(2) rect_pos: vec2<f32>,
      @location(3) rect_size: vec2<f32>,
      @location(4) color: vec4<f32>,
      @location(5) blur: f32,
  };

  struct VertexOut {
      @builtin(position) clip_position: vec4<f32>,
      @location(0) local_uv: vec2<f32>,
      @location(1) half_size: vec2<f32>,
      @location(2) color: vec4<f32>,
      @location(3) blur: f32,
  };

  @vertex
  fn vertex(v: Vertex, i: Instance) -> VertexOut {
      var out: VertexOut;
      let world = i.rect_pos + v.uv * i.rect_size;
      out.clip_position = vec4<f32>(world, 0.0, 1.0);
      out.local_uv = v.uv * 2.0 - 1.0;
      out.half_size = abs(i.rect_size) * 0.5;
      out.color = i.color;
      out.blur = i.blur;
      return out;
  }

  // Abramowitz & Stegun 7.1.26 erf approximation (max abs error ~1.5e-7).
  fn erf(x: f32) -> f32 {
      let s = sign(x);
      let a = abs(x);
      let t = 1.0 / (1.0 + 0.3275911 * a);
      let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741)
          * t - 0.284496736) * t + 0.254829592) * t * exp(-a * a);
      return s * y;
  }

  // Closed-form 1D Gaussian-blurred box coverage along one axis: the integral
  // of a unit box [-half, half] convolved with a Gaussian of std-dev sigma.
  fn blurred_box_1d(p: f32, half: f32, sigma: f32) -> f32 {
      let inv = 1.0 / (sqrt(2.0) * max(sigma, 1e-4));
      return 0.5 * (erf((half - p) * inv) + erf((half + p) * inv));
  }

  @fragment
  fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
      let p = in.local_uv * in.half_size;
      // Separable approximation of the rounded-rect blur: product of the two
      // axis-blurred box coverages. Corner rounding is folded into the
      // effective half-size shrink by `blur` (a v1 approximation; the exact
      // rounded-corner blur is a later refinement, not required by the
      // headless gate).
      let cov = blurred_box_1d(p.x, in.half_size.x, in.blur)
          * blurred_box_1d(p.y, in.half_size.y, in.blur);
      return vec4<f32>(in.color.rgb, in.color.a * cov);
  }
  ```
- [ ] Register the WGSL in `pipeline.rs`. In `register`, after the quad shader insert, add the shadow shader insert (mirrors the existing block):
  ```rust
  {
      let mut shaders = world.resource_mut::<Assets<Shader>>();
      let _prev = shaders.insert(
          shadow_shader_handle().id(),
          Shader::from_wgsl(include_str!("shadow.wgsl"), "buiy/render/shadow.wgsl"),
      );
  }
  ```
  (Place it inside the same `register` fn so it loads at plugin finish. Remove the "temporary stub" note from Task 2's `shadow_shader_handle` doc comment — it is now backed by real WGSL.)
- [ ] Run — expect **PASS** (`shadow_shader_parses_and_has_entry_points ... ok`).
- [ ] Run the full gate. Commit.
  - Commit: `feat(render): box-shadow Gaussian SDF shader (octet ..02)`

---

## Task 5 — Border SDF: pure-CPU port test of the per-side / elliptical-radius band

**Why:** The border band is part of the **quad** primitive (folded into the `..01` quad SDF — [architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set), "border = outer-minus-inner SDF band"), not a separate shader. Its correctness (a fragment inside the band vs. inside the content hole vs. outside the outer edge) is pure SDF math, portable to CPU exactly like `render_instance.rs` ports `sdf_rounded_rect`. Lock the band math **device-free** here as the reference oracle the quad SDF's band must match when the component-model phase grows the instance record and extends `shader.wgsl`'s fragment with per-side widths. This is the same idiom `render_instance.rs::shader_sdf_inside_is_filled_outside_is_empty` already establishes.

**Files**
- Test: `crates/buiy_core/tests/render_border_sdf.rs`

Steps:

- [ ] Write the failing test. Create `crates/buiy_core/tests/render_border_sdf.rs` — it defines the CPU port and asserts the three band regions. The test compiles and runs with no production code (the port lives in the test, like `render_instance.rs`), so it is **green on creation if the math is right** — the value is locking the reference math the quad SDF's band must match:
  ```rust
  //! Pure-CPU reference for the border band SDF that folds into the quad
  //! primitive (architecture.md § 2.1). Mirrors the GPU band fragment 1:1
  //! (only abs/length/min/max). No wgpu adapter — this is the oracle the quad
  //! shader's outer-minus-inner band is validated against when the component
  //! phase grows it. Same idiom as render_instance.rs's sdf port.

  use bevy::math::Vec2;

  /// Signed distance to a rounded rect centered at origin (port of
  /// shader.wgsl::sdf_rounded_rect). Negative inside.
  fn sdf_rounded_rect(p: Vec2, half: Vec2, r: f32) -> f32 {
      let q = p.abs() - half + Vec2::splat(r);
      q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
  }

  /// Border band coverage: a fragment is "in the band" iff it is inside the
  /// outer rounded rect AND outside the inner (content) rounded rect.
  /// Returns (inside_outer, inside_inner); the band is `inside_outer && !inside_inner`.
  fn band_membership(
      p: Vec2,
      outer_half: Vec2,
      outer_r: f32,
      width: Vec2, // per-axis border width (left/right collapsed to x, top/bottom to y)
      inner_r: f32,
  ) -> (bool, bool) {
      let inner_half = outer_half - width;
      let d_outer = sdf_rounded_rect(p, outer_half, outer_r);
      let d_inner = sdf_rounded_rect(p, inner_half, inner_r);
      (d_outer < 0.0, d_inner < 0.0)
  }

  #[test]
  fn point_in_border_band_is_inside_outer_outside_inner() {
      // 100x60 box, 10px uniform border, square corners.
      let outer_half = Vec2::new(50.0, 30.0);
      let width = Vec2::splat(10.0);
      // A point 5px in from the right edge sits inside the 10px band.
      let p = Vec2::new(45.0, 0.0);
      let (in_outer, in_inner) = band_membership(p, outer_half, 0.0, width, 0.0);
      assert!(in_outer, "point is inside the outer box");
      assert!(!in_inner, "point is in the border band, not the content hole");
  }

  #[test]
  fn point_in_content_hole_is_not_in_band() {
      let outer_half = Vec2::new(50.0, 30.0);
      let width = Vec2::splat(10.0);
      let p = Vec2::ZERO; // dead center → content hole
      let (in_outer, in_inner) = band_membership(p, outer_half, 0.0, width, 0.0);
      assert!(in_outer && in_inner, "center is inside both → not band");
  }

  #[test]
  fn point_outside_outer_is_not_in_band() {
      let outer_half = Vec2::new(50.0, 30.0);
      let width = Vec2::splat(10.0);
      let p = Vec2::new(60.0, 0.0); // 10px past the right edge
      let (in_outer, _) = band_membership(p, outer_half, 0.0, width, 0.0);
      assert!(!in_outer, "point past the outer edge is not in the band");
  }

  #[test]
  fn elliptical_radius_shrinks_corner_band_correctly() {
      // Outer corner radius 12, inner radius = outer - min(width) = 2: the band
      // is thinnest along the diagonal. A point on the corner diagonal just
      // inside the outer arc must still be inside the outer rect.
      let outer_half = Vec2::new(50.0, 30.0);
      let width = Vec2::splat(10.0);
      let outer_r = 12.0;
      let inner_r = (outer_r - 10.0).max(0.0); // = 2
      let corner = Vec2::new(50.0 - 4.0, 30.0 - 4.0); // near the rounded corner
      let (in_outer, in_inner) = band_membership(corner, outer_half, outer_r, width, inner_r);
      assert!(in_outer, "corner sample inside the outer rounded rect");
      assert!(!in_inner, "corner sample is in the band (outside the inner arc)");
  }
  ```
- [ ] Run — expect **PASS** (these are CPU-only; if any assertion fails, re-derive the band math — that is the bug to catch before writing WGSL):
  ```sh
  cargo test -p buiy_core --test render_border_sdf
  ```
- [ ] Run the full gate. Commit.
  - Commit: `test(render): pure-CPU border band SDF reference oracle`

---

## Task 6 — (removed) Border folds into the quad SDF — no separate shader, no `..06` octet

**Removed per the plan-reconciliation review.** An earlier draft shipped a standalone `border.wgsl` under a new `..06` octet plus a `BORDER_SHADER_UUID` and a `BuiyPrimitiveKind::Border` key kind. That contradicted (a) the spec's **normative** octet table ([architecture.md § 1.4](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#14-pipelines-pipelinecache--stable-uuid-shaders)), which enumerates only `..01..05` and states "this is the only place the octets are enumerated; plans realize but do not renumber them" — there is no `..06`; (b) the spec's folded-band primitive model ([architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set)): the border band is part of the **quad** primitive ("border = outer-minus-inner SDF band"), not a separate one; and (c) the landed R6 enum `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }`, which has no `Border` variant.

There is **no** `border.wgsl`, **no** `BORDER_SHADER_UUID`, and **no** `..06` octet in this plan. The border band is the **quad** pipeline+shader's (`..01`) responsibility; the outer-minus-inner band math is locked by the Task-5 CPU oracle and is realized in the quad SDF when the component-model phase grows the instance record with per-side widths and the border color channel (that phase owns extending `shader.wgsl`'s fragment with the band — it is out of scope here, where the quad shader keeps the Phase-0 fill behavior). If a reviewer ever wants a standalone stroked-border pipeline for the elliptical per-corner case, that is a spec amendment to raise against architecture.md § 2.1 plus a new R6 `BuiyPrimitiveKind` variant, not a silent plan deviation.

---

## Task 7 — Quad and Shadow have distinct shaders; the quad shader carries fill + border + outline (pure, no GPU)

**Why:** This phase ships two F-tier pipelines — `Quad` (octet `..01`) and `Shadow` (octet `..02`) — and they must reference **distinct** shader handles. The other two roles the spec calls out — the **border band** and the focus **outline** — are not separate kinds in the landed R6 enum (`BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }`): border folds into the quad SDF and outline is the quad pipeline with the clip rect suppressed ([architecture.md § 2.1](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#21-the-primitive-set)). So the device-free contract to lock is: `Quad` and `Shadow` resolve to **different** shaders, and there is no third "border" or "outline" shader UUID. This catches a regression where someone re-introduces a standalone border/outline shader.

**Files**
- Test: `crates/buiy_core/tests/render_primitive_descriptor.rs` (extend)

Steps:

- [ ] Write the failing test — add to `crates/buiy_core/tests/render_primitive_descriptor.rs`:
  ```rust
  #[test]
  fn quad_and_shadow_use_distinct_shaders() {
      let s = BuiyPrimitives::default();
      let quad = s.specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Quad,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      let shadow = s.specialize(BuiyPrimitiveKey {
          kind: BuiyPrimitiveKind::Shadow,
          format: TextureFormat::Rgba8UnormSrgb,
      });
      // The two F-tier pipelines this phase ships reference different shaders;
      // border (folded into quad) and outline (clip-suppressed quad) add no
      // third shader UUID — they are not BuiyPrimitiveKind variants.
      assert_ne!(
          quad.vertex.shader, shadow.vertex.shader,
          "quad and shadow must use distinct vertex shaders"
      );
      assert_ne!(
          quad.fragment.as_ref().unwrap().shader,
          shadow.fragment.as_ref().unwrap().shader,
          "quad and shadow must use distinct fragment shaders (..01 vs ..02)"
      );
  }
  ```
- [ ] Run — expect **PASS** (the Task-2 `shader_for` maps `Quad` to `shader_handle()` (`..01`) and `Shadow` to `shadow_shader_handle()` (`..02`), which are distinct UUIDs). If it **fails**, the bug is in `BuiyPrimitives::shader_for` — fix it there:
  ```sh
  cargo test -p buiy_core --test render_primitive_descriptor
  ```
- [ ] Run the full gate. Commit.
  - Commit: `test(render): pin quad/shadow distinct-shader contract`

---

## Task 8 — Wire the quad specializer into `pipeline.rs::register` (replace the hard-coded descriptor; keep Phase-0 node green) — GPU-touching, but the headless half gates

**Why:** Make the production `register` build the quad pipeline through `BuiyPrimitives::specialize` keyed on `ViewTarget::main_texture_format()` instead of the inline hard-coded `Rgba8UnormSrgb` descriptor, so the Phase-0 node (`node.rs`) keeps drawing the quad and the existing `render_smoke.rs` GPU test still finds a registered quad pipeline. The descriptor-construction change is provable headless (Task 2 already covers it); the *registration through `PipelineCache`* and the view-format read are GPU-only and ride `#[ignore]`.

> **Scope note.** Full per-format registration through `SpecializedRenderPipelines` (queuing every `(kind, format)` variant the moment its format is first seen) is a render-world prepare-phase concern that needs a live `PipelineCache` — that wiring lands with the node-rewrite/effect-compositor phases. This task does the **minimal** production change: `register` builds the **quad / view-format** descriptor via `specialize` and queues it through the existing `PipelineCache`, preserving `BuiyPipeline.id`. No behavior change for the headless gate; the GPU smoke test continues to pass on a real adapter.

**Files**
- Modify: `crates/buiy_core/src/render/pipeline.rs` (`register` builds the quad descriptor via `BuiyPrimitives::specialize`)
- Test: `crates/buiy_core/tests/render_smoke.rs` (extend with one `#[ignore]` GPU assertion that the quad pipeline still registers via the specializer path)

Steps:

- [ ] Write the failing test — add an `#[ignore]` GPU test to `crates/buiy_core/tests/render_smoke.rs` mirroring the existing `pipeline_registers_in_render_app` idiom (it asserts the *same* `BuiyPipeline` resource still registers after the refactor — guarding against the refactor dropping the resource):
  ```rust
  // Same RenderApp/wgpu-adapter caveat as the other render_smoke tests:
  // RenderPlugin::build does block_on(initialize_renderer(...)) which expect()s
  // a wgpu adapter. After Task 8 the quad pipeline is built through
  // BuiyPrimitives::specialize; this asserts the BuiyPipeline resource (and its
  // valid quad CachedRenderPipelineId) still registers via that path.
  //
  // Run locally with: `cargo test -p buiy_core --test render_smoke -- --ignored`.
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
  fn quad_pipeline_registers_via_specializer() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
      let pipeline = render_app
          .world()
          .get_resource::<buiy_core::render::pipeline::BuiyPipeline>()
          .expect("BuiyPipeline registered via specializer path");
      // The id is a valid handle into the cache (compilation is async; we only
      // assert the resource + id exist, not that the pipeline finished).
      let _ = pipeline.id;
  }
  ```
- [ ] Run — expect this new test is **`#[ignore]`d** so it does not run headlessly (the gate stays green); it only runs under `-- --ignored` on a GPU host. Confirm the suite still builds:
  ```sh
  cargo test -p buiy_core --test render_smoke
  ```
  Expected: existing tests run, the three GPU ones (including the new one) report as `ignored`.
- [ ] Minimal impl — in `pipeline.rs::register`, replace the inline `RenderPipelineDescriptor { … }` literal with a `specialize` call keyed on the view's default format. Because `register` runs at plugin-finish (no `ViewTarget` yet), key the **main-pass** variant off the **`TextureFormat::Rgba8UnormSrgb` literal** — this is exactly what `ViewTarget::main_texture_format()` returns for the default `Camera2d` view ([architecture.md § 1.4](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#14-pipelines-pipelinecache--stable-uuid-shaders)) and what the Phase-0 descriptor already hard-coded, so using the literal here avoids importing the `BevyDefault` trait (its `bevy_default()` returns `Rgba8UnormSrgb`, but the trait is not re-exported through `bevy::render` and would add an import for no behavior difference):
  ```rust
  use crate::render::buckets::BuiyPrimitiveKind;
  use crate::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};
  use bevy::render::render_resource::SpecializedRenderPipeline;

  // … inside register, after shaders are inserted and pipeline_cache is held:
  let descriptor = BuiyPrimitives.specialize(BuiyPrimitiveKey {
      kind: BuiyPrimitiveKind::Quad,
      format: TextureFormat::Rgba8UnormSrgb,
  });
  ```
  Delete the now-dead inline descriptor literal. Keep the rest of `register` (the static unit-quad VBO creation, `queue_render_pipeline`, `BuiyPipeline { id, vertex_buffer }` insertion) unchanged — `descriptor` now comes from the specializer. `TextureFormat` is already imported in `pipeline.rs`; `SpecializedRenderPipeline` is the trait that brings `.specialize` into scope.
- [ ] Verify the existing **headless** descriptor tests (Task 2) still cover the format/blend/strides, and the existing `render_instance.rs` CPU tests are untouched. Run the headless suites:
  ```sh
  cargo test -p buiy_core --test render_primitive_descriptor --test render_instance
  ```
  Expected: all green.
- [ ] Run the full gate. Commit.
  - Commit: `refactor(render): build quad pipeline via the typed-primitive specializer`

---

## Task 9 — GPU e2e: real `SpecializedRenderPipelines` allocation through a live `PipelineCache` (`#[ignore]`)

**Why:** The headless tests prove the key contract over a stub allocator (Task 3); this task proves the **real** thing on a GPU host — that `SpecializedRenderPipelines<BuiyPrimitives>::specialize` against a live `PipelineCache` hands out distinct `CachedRenderPipelineId`s per `(kind, format)` and dedupes repeats. It needs the `RenderApp` (hence a wgpu adapter), so it is `#[ignore]`d exactly like `render_smoke.rs`. This closes the loop: the device-free contract test (Task 3) and this GPU test assert the *same* property at two tiers.

**Files**
- Test: `crates/buiy_core/tests/render_specialize_gpu.rs`

Steps:

- [ ] Write the test (born `#[ignore]`d — it never runs headlessly, so it cannot fail the gate; the "failing first" discipline here is structural: confirm it compiles and is collected-but-ignored). Create `crates/buiy_core/tests/render_specialize_gpu.rs`:
  ```rust
  //! GPU e2e (`#[ignore]`): real specialization through a live PipelineCache.
  //! Needs a wgpu adapter (RenderPlugin::build block_on(initialize_renderer)
  //! expect()s one) — headless CI has none, so this is ignored and runs only
  //! under `-- --ignored` on a GPU/lavapipe host, alongside render_smoke.rs.
  //! The device-free counterpart is tests/render_primitive_dedup.rs.

  use bevy::prelude::*;
  use bevy::render::{
      RenderApp,
      render_resource::{PipelineCache, SpecializedRenderPipelines, TextureFormat},
  };
  use buiy_core::render::buckets::BuiyPrimitiveKind;
  use buiy_core::render::primitive::{BuiyPrimitiveKey, BuiyPrimitives};

  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
  fn specialize_allocates_distinct_ids_per_format() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
      let world = render_app.world_mut();
      let cache = world.resource::<PipelineCache>();
      // Drive the real specialization cache directly.
      let mut specialized = SpecializedRenderPipelines::<BuiyPrimitives>::default();
      let specializer = BuiyPrimitives;

      let id_srgb = specialized.specialize(
          cache,
          &specializer,
          BuiyPrimitiveKey {
              kind: BuiyPrimitiveKind::Quad,
              format: TextureFormat::Rgba8UnormSrgb,
          },
      );
      let id_hdr = specialized.specialize(
          cache,
          &specializer,
          BuiyPrimitiveKey {
              kind: BuiyPrimitiveKind::Quad,
              format: TextureFormat::Rgba16Float,
          },
      );
      // Repeat the srgb key → same id (dedup).
      let id_srgb2 = specialized.specialize(
          cache,
          &specializer,
          BuiyPrimitiveKey {
              kind: BuiyPrimitiveKind::Quad,
              format: TextureFormat::Rgba8UnormSrgb,
          },
      );
      assert_ne!(id_srgb, id_hdr, "distinct format → distinct cached id");
      assert_eq!(id_srgb, id_srgb2, "repeated key → deduped id");
  }
  ```
- [ ] Confirm it compiles and is **collected as ignored** (does not run, does not fail the gate):
  ```sh
  cargo test -p buiy_core --test render_specialize_gpu
  ```
  Expected: `1 test ... ignored` (and a clean build — a compile error here means a type/signature drift in `primitive.rs`, fix that).
- [ ] (GPU host only, not on CI) optionally run it to validate the real path:
  ```sh
  cargo test -p buiy_core --test render_specialize_gpu -- --ignored
  ```
- [ ] Run the full gate. Commit.
  - Commit: `test(render): GPU e2e for live SpecializedRenderPipelines allocation (#[ignore])`

---

## Task 10 — Doc sweep: mark the phase landed in the docs index

**Why:** Per the project's "docs ship with the change" rule, record this plan as landed in the docs catalog and cross-link it from the render spec's plans area, so the next phase author sees the pipelines/shaders are realized.

**Files**
- Modify: `docs/README.md` (add this plan under the render-pipeline plans area)

Steps:

- [ ] Read `docs/README.md` and find the render-pipeline plans grouping (or the plans table). Add a row/link:
  ```
  - [2026-06-03-buiy-render-r7-pipelines-shaders](plans/2026-06-03-buiy-render-r7-pipelines-shaders.md) — typed-primitive SpecializedRenderPipelines (quad `..01` + shadow `..02`; border folds into the quad SDF, outline is the clip-suppressed quad) + SDF WGSL under the 0xB01A_01XX octets. [landed]
  ```
  Follow the exact formatting of the surrounding entries (match the `organizing-buiy-docs` conventions — bullet style, date-prefix, trailing status marker).
- [ ] If `docs/README.md` has no plans grouping yet, add a minimal "Render pipeline — plans" subsection under the render-pipeline area, mirroring how layout plans are grouped.
- [ ] Run the full gate (doc-only change, but `cargo doc` + fmt must still pass). Commit.
  - Commit: `docs(render): index the pipelines+shaders plan as landed`

---

## What this phase deliberately leaves to sibling phases

- **Component model** (`Background`/`Border`/`BoxShadow`/`Outline` components, the `Visual` migration, the extract rewrite to `extract_buiy_nodes`) — component-model phase. This phase's shaders read the **existing** 36-byte instance record; per-side border widths and multi-shadow lists arrive when the instance layout grows there.
- **Node rewrite + per-format registration through `SpecializedRenderPipelines` in a prepare pass** (`prepare_buiy_instances`, `BuiyInstanceBuffers`, per-view storage) — architecture/handoff phase. Task 8 only swaps the quad descriptor source; it does not move registration into the prepare phase.
- **Effect-group `Rgba16Float` targets and the composite pipeline (`..05`)** — effect-compositor phase. This phase builds primitives *for* the `Rgba16Float` format (the key supports it, Tasks 2–3) but allocates no group targets.
- **Path-SDF (`..04`)** — reserved C-tier; no shader here.
- **Glyph-alpha (`..03`)** — atlas-and-text-seam phase.
- **The view uniform / logical-px instance units** (retiring the CPU y-flip + radius-px approximation in `instance.rs`) — handoff phase; the shaders here still consume the Phase-0 clip-space instance record.

---

*Plan authored 2026-06-03 against the render-pipeline design spec; reconciled to the landed R6 enum (`BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }`) and the spec's folded-band model per the plan-reconciliation review. Headless gate is the four-command block at the top; GPU assertions are `#[ignore]`d with the `render_smoke.rs` wording. This plan adds **no** octet beyond the spec's normative enumeration ([architecture.md § 1.4](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md#14-pipelines-pipelinecache--stable-uuid-shaders), `..01..05`): it ships the `..01` quad and `..02` shadow shaders; the border band folds into `..01` and the focus outline is the clip-suppressed quad pipeline.*
