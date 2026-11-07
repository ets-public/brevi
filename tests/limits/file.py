# Writes a 10 MB file where the caller allows 1 MB.
with open("limits-file.out", "w") as f:
    for i in range(100000):
        f.write("x" * 99 + "\n")
print("not reached")
