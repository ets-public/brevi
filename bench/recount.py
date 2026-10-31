"""Count every saved solution (solutions/ and edit/solutions/) and every saved error output
(logs/attempts/) both ways: the old estimate and exact tokens. Writes logs/recount.json."""
import glob, json, os, re, collections
from concurrent.futures import ThreadPoolExecutor
import tokcount

def est(t): return len(re.findall(r"[A-Za-z]+|\d+|\s{2,}|[^\sA-Za-z\d]", t))
files = sorted(f for p in ("solutions/*/*", "edit/solutions/*/*", "logs/attempts/*/*") for f in glob.glob(p) if os.path.isfile(f))
texts = [open(f, errors="replace").read() for f in files]
with ThreadPoolExecutor(8) as ex: exact = list(ex.map(tokcount.count, texts))
rows = [{"file": f, "est": est(t), "exact": n} for f, t, n in zip(files, texts, exact)]
json.dump(rows, open("logs/recount.json", "w"), indent=0)
by = collections.defaultdict(lambda: [0, 0, 0])
for r in rows:
    f = r["file"]
    kind = "error output" if f.endswith(".out") else {".ax": "brevi", ".bv": "brevi", ".py": "python", ".rs": "rust"}.get(os.path.splitext(f)[1], "other")
    b = by[kind]; b[0] += 1; b[1] += r["est"]; b[2] += r["exact"]
print(f"{'kind':14} files estimate exact exact/est")
for k, (n, e, x) in sorted(by.items()): print(f"{k:14} {n:5} {e:8} {x:6} {x/e:.3f}")
