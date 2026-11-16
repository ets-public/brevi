import subprocess

# Spends much of its CPU time in kernel mode (each os.stat is a file-system call), which the job's own CPU
# limit does not count. The child prints its own user and kernel seconds as it goes; check.ps1 reads the last
# line: stopped at the limit of the two together, before its user time alone reached it.
child = """
import os, time
last = 0.0
while True:
    os.stat('.')
    now = time.monotonic()
    if now - last > 0.02:
        last = now
        t = os.times()
        print(f"{t.user:.3f} {t.system:.3f}", flush=True)
"""
r = subprocess.run(["python", "-c", child])
raise SystemExit(r.returncode or 1)
