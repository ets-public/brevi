import json, glob, collections, sys
rows = []
for f in sorted(glob.glob(sys.argv[1] if len(sys.argv) > 1 else "logs/*.jsonl")):
    L = [json.loads(l) for l in open(f)]
    by = collections.OrderedDict()
    for r in L: by.setdefault(r["task"], []).append(r)
    run = L[0]["run"]
    solved = sum(1 for t in by.values() if t[-1]["pass"] == t[-1]["total"] and t[-1]["total"] > 0)
    first = sum(1 for t in by.values() if t[0]["pass"] == t[0]["total"] and t[0]["total"] > 0)
    final_tok = sum(t[-1]["code_tok"] for t in by.values())
    written = sum(r["code_tok"] for r in L)
    read = sum(r["out_tok"] for r in L)
    rows.append((run, len(by), solved, first, len(L), final_tok, written, read, written + read))
print(f"{'run':14} tasks solved 1st-try runs final_tok written err_read loop_total")
for r in rows: print(f"{r[0]:14} {r[1]:5} {r[2]:6} {r[3]:8} {r[4]:4} {r[5]:9} {r[6]:7} {r[7]:8} {r[8]:10}")
