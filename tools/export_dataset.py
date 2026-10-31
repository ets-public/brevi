"""Writes the benchmark as a dataset: every task prompt, and every solution that passes on today's compiler.
usage: python3 tools/export_dataset.py [OUT_DIR]   (default dist/dataset)

Solutions come from compat/baseline.txt, the list CI re-checks, so each one is known to pass its tests.
Old Axon-era files (.ax) are Brevi too and are exported as brevi. Identical programs are kept once.
Output: tasks.jsonl (task, set, prompt, tests), solutions.jsonl (task, lang, model, run, files) and README.md,
ready for a Hugging Face dataset repo. Nothing is uploaded.
"""
import hashlib, json, os, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "bench"
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "dist" / "dataset"
LANG = {".ax": "brevi", ".bv": "brevi", ".py": "python", ".rs": "rust"}

# Task sets as the benchmark names them (bench/subject_prompt.py).
sets = {}
src = (BENCH / "subject_prompt.py").read_text()
for name in ("SMALL", "BIG", "NEW", "XL"):
    m = re.search(rf'^{name} = "([^"]+)"', src, re.M)
    for t in (m[1].split() if m else []):
        sets[t] = name.lower()

OUT.mkdir(parents=True, exist_ok=True)
tasks = []
for d in sorted((BENCH / "tasks").iterdir()):
    if not (d / "prompt.md").exists():
        continue
    tests = [{"input": i.read_text(), "output": i.with_suffix(".out").read_text()}
             for i in sorted((d / "tests").glob("*.in")) if i.with_suffix(".out").exists()]
    tasks.append({"task": d.name, "set": sets.get(d.name, "other"), "prompt": (d / "prompt.md").read_text(), "tests": tests})

seen, sols = set(), []
for line in (ROOT / "compat" / "baseline.txt").read_text().split():
    p = ROOT / line
    run = p.parent.name if p.parent.parent.name == "solutions" else p.parent.parent.name
    if p.is_dir() or not p.exists():
        continue
    multi = p.parent.parent.name != "solutions"
    task = p.parent.name if multi else p.stem
    files = {f.name: f.read_text() for f in sorted(p.parent.iterdir()) if f.suffix == p.suffix} if multi else {p.name: p.read_text()}
    key = hashlib.sha256(json.dumps([task, files], sort_keys=True).encode()).hexdigest()
    if key in seen:
        continue
    seen.add(key)
    model = next((m for m in ("sonnet", "haiku", "opus") if m in run), "")
    sols.append({"task": task, "lang": LANG[p.suffix], "model": model, "run": run, "files": files})

with open(OUT / "tasks.jsonl", "w") as f:
    for t in tasks:
        f.write(json.dumps(t) + "\n")
with open(OUT / "solutions.jsonl", "w") as f:
    for s in sols:
        f.write(json.dumps(s) + "\n")

count = {}
for s in sols:
    count[s["lang"]] = count.get(s["lang"], 0) + 1
(OUT / "README.md").write_text(f"""---
license: apache-2.0
language: [en]
tags: [code, brevi, benchmark, program-synthesis]
pretty_name: Brevi benchmark tasks and verified solutions
---

# Brevi benchmark tasks and verified solutions

{len(tasks)} command-line programming tasks (stdin to stdout, with tests) and {len(sols)} model-written solutions
that pass every test: {", ".join(f"{n} {l}" for l, n in sorted(count.items()))}. Brevi is a programming language
for AI models that compiles to Rust (https://brevi-lang.org). The solutions were written by Claude models during
the Brevi benchmark, which counts the tokens a model spends to finish each task.

- `tasks.jsonl`: `task`, `set` (small, big, new = held out, xl), `prompt`, `tests` (input and expected output)
- `solutions.jsonl`: `task`, `lang`, `model`, `run`, `files` (file name to source)

Every solution is re-checked against the current compiler in CI, so all of them still pass. Multi-file solutions are not yet in that check, so they are left out.
""")
print(f"{len(tasks)} tasks, {len(sols)} solutions ({count}) in {OUT}")
