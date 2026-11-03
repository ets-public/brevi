import sys
data = sys.stdin.read().split()
total = 0
for tok in data:
    try:
        total += 100 // int(tok)
    except ValueError:
        print("bad", tok)
    except ZeroDivisionError:
        print("zero")
d = {"a": 1}
try:
    print(d["b"])
except KeyError as e:
    print("missing", e)
try:
    [1, 2][5]
except IndexError as e:
    print("index:", e)
print(total)
raise SystemExit(3)
