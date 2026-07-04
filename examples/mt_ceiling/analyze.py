#!/usr/bin/env python3
"""Analyze the mt_ceiling sweep CSV into ceiling tables."""
import csv, sys
from collections import defaultdict

path = sys.argv[1]
rows = []
with open(path) as f:
    for r in csv.DictReader(f):
        if not r.get("label") or r["label"].startswith("__"):
            continue
        for k in ("threads_req", "par_cost", "dirty", "pool_threads"):
            r[k] = int(r[k])
        for k in ("p50_us", "p90_us", "mean_us", "min_us"):
            r[k] = float(r[k])
        rows.append(r)

def get(label, threads):
    for r in rows:
        if r["label"] == label and r["threads_req"] == threads:
            return r
    return None

THREADS = sorted({r["threads_req"] for r in rows})

def ms(v): return f"{v/1000:8.2f}"

def curve_table(regime):
    print(f"\n===== CEILING CURVES — {regime} load — frame p50 (ms) vs threads =====")
    configs = [
        (f"bare_{regime}",  "bare (no Buiy)"),
        (f"flatS_{regime}", "Buiy flat static"),
        (f"flatD_{regime}", "Buiy flat DIRTY"),
        (f"textS_{regime}", "Buiy text static"),
        (f"textD_{regime}", "Buiy text DIRTY"),
    ]
    hdr = "  ".join(f"t={t:<2}" for t in THREADS)
    print(f"{'config':<20} {hdr}")
    base = {}
    for lbl, name in configs:
        cells = []
        for t in THREADS:
            r = get(lbl, t)
            cells.append(ms(r["p50_us"]) if r else "   n/a  ")
            if lbl == f"bare_{regime}":
                base[t] = r["p50_us"] if r else None
        print(f"{name:<20} " + " ".join(cells))
    # Speedup of bare vs Buiy-text-dirty
    print(f"\n  -- speedup (T@min_threads / T@threads) --")
    for lbl, name in configs:
        r1 = get(lbl, THREADS[0]);
        cells = []
        for t in THREADS:
            r = get(lbl, t)
            if r and r1:
                cells.append(f"{r1['p50_us']/r['p50_us']:6.2f}x")
            else:
                cells.append("  n/a ")
        print(f"{name:<20} " + " ".join(cells))

def floor_table():
    print(f"\n===== BUIY SERIAL FLOOR S (no user work) — p50 (ms) vs threads =====")
    print("  (thread-INVARIANCE = un-parallelizable serial cost; H-B)")
    configs = [("floor_flatS","flat static"),("floor_flatD","flat dirty"),
               ("floor_textS","text static"),("floor_textD","text dirty")]
    hdr = "  ".join(f"t={t:<2}" for t in THREADS)
    print(f"{'config':<16} {hdr}")
    for lbl, name in configs:
        cells = [ms(get(lbl,t)["p50_us"]) if get(lbl,t) else "  n/a " for t in THREADS]
        print(f"{name:<16} " + " ".join(cells))

def overlap_table(regime):
    # Does Buiy just ADD serial cost (good citizen), or STALL the parallel work?
    # expected_additive = bare + floor ; expected_ideal_overlap = max(bare, floor)
    print(f"\n===== OVERLAP CHECK — {regime} — measured vs additive vs overlap (ms) =====")
    print("  measured≈max→perfect overlap; measured≈sum→no overlap (barriers)")
    for cfg, floor in [("flatS","floor_flatS"),("textS","floor_textS"),
                       ("flatD","floor_flatD"),("textD","floor_textD")]:
        print(f"  -- {cfg} --")
        for t in THREADS:
            b = get(f"bare_{regime}", t); m = get(f"{cfg}_{regime}", t); fl = get(floor, t)
            if not (b and m and fl): continue
            bare=b["p50_us"]; meas=m["p50_us"]; S=fl["p50_us"]
            add=bare+S; mx=max(bare,S)
            print(f"    t={t:<2} bare={ms(bare)} +S={ms(S)}  measured={ms(meas)}  "
                  f"[max={ms(mx)} sum={ms(add)}]")

def tax_table():
    print(f"\n===== ST vs MT EXECUTOR TAX (p50 ms) =====")
    for t in (1,8):
        for a,b,name in [("tax_floorFlat_st","tax_floorFlat_mt","Buiy flat floor"),
                         ("tax_bareHeavy_st","tax_bareHeavy_mt","bare heavy par")]:
            ra=get(a,t); rb=get(b,t)
            if ra and rb:
                print(f"  t={t} {name:<18} ST={ms(ra['p50_us'])}  MT={ms(rb['p50_us'])}  "
                      f"MT/ST={rb['p50_us']/ra['p50_us']:.2f}x")

for reg in ("mod","heavy"):
    curve_table(reg)
floor_table()
for reg in ("mod","heavy"):
    overlap_table(reg)
tax_table()
print()
