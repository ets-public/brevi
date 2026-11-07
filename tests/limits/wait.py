# Waits without using the CPU: only the wall-clock limit stops it.
import time
print("waiting")
time.sleep(60)
print("never printed")
