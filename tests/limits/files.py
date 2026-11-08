# Creates 5 files where the caller allows 3.
for i in range(5):
    with open(f"limits-files-{i}.out", "w") as f:
        f.write("x")
    print("created", i)
