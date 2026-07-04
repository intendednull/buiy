#!/usr/bin/env bash
# Follow-up measurements: (1) serial floor S scales with UI size (thread-invariant);
# (2) overlap isolation — combined update-phase (EXTRACT=0) vs bare, to test H-C.
set -euo pipefail
OUT="${1:?usage: run_followups.sh <out.csv> [bin]}"
BIN="${2:-./target/release/mt_ceiling}"
[[ -x "$BIN" ]] || { echo "binary not found: $BIN" >&2; exit 1; }
W=32; F=120; PE=4000

BUIY_MT_HEADER=1 BUIY_MT_FRAMES=1 BUIY_MT_WARMUP=1 BUIY_MT_PAR_COST=0 \
  BUIY_MT_BUIY=0 BUIY_MT_THREADS=1 BUIY_MT_LABEL=__hdr "$BIN" | head -1 > "$OUT"

run() { # label buiy ui exec threads cost dirty [extra env...]
  local l="$1" b="$2" u="$3" e="$4" t="$5" c="$6" d="$7"; shift 7
  echo "  $l t=$t $*" >&2
  env "$@" BUIY_MT_LABEL="$l" BUIY_MT_BUIY="$b" BUIY_MT_UI="$u" BUIY_MT_EXEC="$e" \
    BUIY_MT_THREADS="$t" BUIY_MT_PAR_COST="$c" BUIY_MT_DIRTY="$d" \
    BUIY_MT_PAR_ENTITIES="$PE" BUIY_MT_WARMUP="$W" BUIY_MT_FRAMES="$F" "$BIN" >> "$OUT"
}

echo "=== Floor S scales with UI size (t=8, no par work), DIRTY ===" >&2
for ui in text_small text_large text_huge; do
  run "sizeS_${ui}" 1 "$ui" mt 8 0 0            # static
  run "sizeD_${ui}" 1 "$ui" mt 8 0 1            # dirty (wholesale re-extract)
  run "sizeD_${ui}_upd" 1 "$ui" mt 8 0 1 BUIY_MT_EXTRACT=0   # update-only (incremental-ideal)
done

echo "=== Overlap isolation: update-phase only (EXTRACT=0), moderate load ===" >&2
for t in 1 2 4 8 16; do
  run "ovl_bare_upd"  0 none       mt "$t" 600 0
  run "ovl_textS_upd" 1 text_large mt "$t" 600 0 BUIY_MT_EXTRACT=0
  run "ovl_flatS_upd" 1 flat_large mt "$t" 600 0 BUIY_MT_EXTRACT=0
done

echo "Done-followups: $(($(wc -l < "$OUT") - 1)) rows → $OUT" >&2
