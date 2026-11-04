def f(x):
    try:
        if x > 0:
            return "pos"
        return "neg"
    finally:
        print("cleanup", x)

print(f(1))
print(f(-1))
for i in range(3):
    try:
        if i == 1:
            continue
        print("body", i)
    finally:
        print("fin", i)
n = 0
while True:
    try:
        n += 1
        if n > 2:
            break
    finally:
        print("w", n)
