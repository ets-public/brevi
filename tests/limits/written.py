# Writes 3 MB in many small files where the caller allows 1 MB written in all (each file is under file=1).
for i in range(30):
    with open("limits-written.out", "w") as f:
        f.write("x" * 100000)
print("not reached")
