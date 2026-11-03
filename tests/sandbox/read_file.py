# No allow line: --sandbox refuses the read on every path (typed, dynamic, python3).
with open("tests/sandbox/data.txt") as f:
    print(f.read())
