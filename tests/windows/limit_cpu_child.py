import subprocess

# A process the program starts is in the same Windows job, so its CPU time counts against the limit too.
subprocess.run(["python", "-c", "while True: pass"])
print("not reached")
