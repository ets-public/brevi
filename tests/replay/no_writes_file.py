# A program that writes a file is not run a second time: an int past 64 bits stops it with an error.
xs = [1]
with open("/dev/null", "w") as f:
    f.write("x")
for i in range(1, 30):
    xs[0] = xs[0] * i
print(xs[0])
