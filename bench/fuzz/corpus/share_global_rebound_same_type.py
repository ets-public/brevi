count = 0
d = {"a": 1}
def bump():
    global count
    count += 1
    d["b"] = count
bump()
count = 10
d = {"z": 0}
bump()
print(count, d)
def run():
    for i in range(2):
        bump()
xs = [1]
def addx(v):
    xs.append(v)
for k in range(3):
    addx(k)
    xs = [k * 10]
    addx(k)
print(xs)
