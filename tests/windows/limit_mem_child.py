import subprocess

# A process the program starts is in the same Windows job, so its memory counts against the limit too.
r = subprocess.run(["python", "-c", "xs = []\nwhile True: xs.append('x' * 1000000)"])
raise SystemExit(r.returncode or 1)
