import subprocess

# A process the program starts is in the same Windows job, so its CPU time counts against the limit too,
# while the program itself only waits.
r = subprocess.run(["python", "-c", "while True: pass"])
raise SystemExit(r.returncode or 1)
