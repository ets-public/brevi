"""Re-scores saved runs as if every retry were a patch (patchtok.py on consecutive saved attempts).
usage: retro_patch.py RUN..."""
import json, os, subprocess, sys
B = os.path.dirname(os.path.abspath(__file__))
for run in sys.argv[1:]:
    L = [json.loads(l) for l in open(f"{B}/logs/{run}.jsonl")]
    seen, total = {}, 0
    for r in L:
        n = seen[r["task"]] = seen.get(r["task"], 0) + 1
        prev, cur = f"{B}/logs/attempts/{run}/{r['task']}.{n - 1}.src", f"{B}/logs/attempts/{run}/{r['task']}.{n}.src"
        pt = int(subprocess.check_output(["python3", f"{B}/patchtok.py", prev, cur])) if n > 1 else r["code_tok"]
        total += pt + r["out_tok"]
    print(f"{run:18} patch_total {total}")
