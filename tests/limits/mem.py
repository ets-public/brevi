# Allocates without end: stopped at 200 MB of address space (Linux).
xs = []
while True:
    xs.append([0] * 100000)
