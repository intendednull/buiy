#!/usr/bin/env bash
# PROTOTYPE sweep driver for the MT-ceiling benchmark (throwaway).
# One process per matrix point (ComputeTaskPool thread count is a per-process
# singleton). Appends CSV rows to $OUT; SPLIT/COUNTERS lines go to stderr.
set -euo pipefail

OUT="${1:?usage: run_sweep.sh <out.csv> [bin]}"
BIN="${2:-./target/release/mt_ceiling}"
[[ -x "$BIN" ]] || { echo "binary not found: $BIN" >&2; exit 1; }

WARMUP="${WARMUP:-32}"
FRAMES="${FRAMES:-150}"
PAR_ENTITIES="${PAR_ENTITIES:-4000}"
THREADS_LIST="${THREADS_LIST:-1 2 4 8 16}"
# Two load regimes: moderate (crosses Buiy's floor at high thread counts) and
# heavy (dwarfs Buiy's floor — shows Buiy is negligible for truly heavy apps).
MOD="${MOD:-600}"
HEAVY="${HEAVY:-3000}"

# header
BUIY_MT_HEADER=1 BUIY_MT_FRAMES=1 BUIY_MT_WARMUP=1 BUIY_MT_PAR_COST=0 \
  BUIY_MT_BUIY=0 BUIY_MT_THREADS=1 BUIY_MT_LABEL=__hdr "$BIN" | head -1 > "$OUT"

run() { # run <label> <buiy> <ui> <exec> <threads> <cost> <dirty> [extra env...]
  local label="$1" buiy="$2" ui="$3" exec="$4" threads="$5" cost="$6" dirty="$7"; shift 7
  echo "  $label: buiy=$buiy ui=$ui exec=$exec t=$threads cost=$cost dirty=$dirty $*" >&2
  env "$@" BUIY_MT_LABEL="$label" BUIY_MT_BUIY="$buiy" BUIY_MT_UI="$ui" BUIY_MT_EXEC="$exec" \
    BUIY_MT_THREADS="$threads" BUIY_MT_PAR_COST="$cost" BUIY_MT_DIRTY="$dirty" \
    BUIY_MT_PAR_ENTITIES="$PAR_ENTITIES" BUIY_MT_WARMUP="$WARMUP" BUIY_MT_FRAMES="$FRAMES" \
    "$BIN" >> "$OUT"
}

for regime in mod heavy; do
  cost=$MOD; [[ $regime == heavy ]] && cost=$HEAVY
  echo "=== Ceiling curves, $regime load (cost=$cost), vs threads ===" >&2
  for t in $THREADS_LIST; do
    run "bare_$regime"        0 none       mt "$t" "$cost" 0
    run "flatS_$regime"       1 flat_large mt "$t" "$cost" 0
    run "flatD_$regime"       1 flat_large mt "$t" "$cost" 1
    run "textS_$regime"       1 text_large mt "$t" "$cost" 0
    run "textD_$regime"       1 text_large mt "$t" "$cost" 1
  done
done

echo "=== Buiy serial floor S (no user work), vs threads ===" >&2
for t in $THREADS_LIST; do
  run "floor_flatS" 1 flat_large mt "$t" 0 0
  run "floor_flatD" 1 flat_large mt "$t" 0 1
  run "floor_textS" 1 text_large mt "$t" 0 0
  run "floor_textD" 1 text_large mt "$t" 0 1
done

echo "=== Extract split (update-only) at t=8, floors ===" >&2
run "split_flatD_upd" 1 flat_large mt 8 0 1 BUIY_MT_EXTRACT=0
run "split_textD_upd" 1 text_large mt 8 0 1 BUIY_MT_EXTRACT=0
run "split_textS_upd" 1 text_large mt 8 0 0 BUIY_MT_EXTRACT=0

echo "=== ST vs MT executor tax (Buiy own serial work, no user work) ===" >&2
for t in 1 8; do
  run "tax_floorFlat_st" 1 flat_large st "$t" 0 0
  run "tax_floorFlat_mt" 1 flat_large mt "$t" 0 0
  run "tax_bareHeavy_st" 0 none       st "$t" "$HEAVY" 0
  run "tax_bareHeavy_mt" 0 none       mt "$t" "$HEAVY" 0
done

echo "Done: $(($(wc -l < "$OUT") - 1)) rows → $OUT" >&2
