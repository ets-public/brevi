import subprocess

# Spends most of its CPU time in kernel mode (each os.stat is a file-system call), which the job's own CPU
# limit does not count: Brevi's watch thread must still stop it at the limit of user and kernel time together.
r = subprocess.run(["python", "-c", "import os\nwhile True: os.stat('.')"])
raise SystemExit(r.returncode or 1)
