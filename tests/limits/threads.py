# --limit threads counts every thread and process the program starts: the kernel refuses the one past it,
# the program fails, and the run reports the limit.
import subprocess

subprocess.run(["sh", "-c", "for i in 1 2 3 4 5 6 7 8; do sleep 1 & done; wait"], check=True)
print("all started")
