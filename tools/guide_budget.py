"""Fails when the core guide (BREVI.md) grows past its budget. usage: guide_budget.py

Every session reads BREVI.md, so each line costs tokens on every task. Details belong in
REFERENCE.md or LIBRARIES.md, which `brevi guide WORD` serves on demand.
With BENCH_ANTHROPIC_KEY set the count is exact (bench/tokcount.py); without it (as in CI)
the check uses bytes, at 2.2 bytes per token, the ratio measured on BREVI.md.
"""
import os, subprocess, sys

TOKENS = 3600
BYTES_PER_TOKEN = 2.2
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
path = os.path.join(ROOT, "BREVI.md")
text = open(path).read()

if os.environ.get("BENCH_ANTHROPIC_KEY"):
    n = int(subprocess.run([sys.executable, os.path.join(ROOT, "bench", "tokcount.py"), path],
                           capture_output=True, text=True, check=True).stdout.split()[0])
    how = "tokens (exact)"
else:
    n = round(len(text.encode()) / BYTES_PER_TOKEN)
    how = "tokens (estimated from bytes)"

print(f"BREVI.md: {n} {how}, budget {TOKENS}")
if n > TOKENS:
    print("Over budget: move details to REFERENCE.md (served by `brevi guide WORD`) or cut a line for each line added.")
    sys.exit(1)
