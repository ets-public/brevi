n = int(input())
parts = []
for i in range(n):
    s = f"item-{i:06d}:{i * 7 % 1000}"
    if s.endswith("7"):
        s = s.upper()
    parts.append(s.replace("-", "_"))
text = ",".join(parts)
fields = text.split(",")
total = 0
for f in fields:
    k, v = f.split(":")
    total += int(v) + len(k)
caps = sum(1 for f in fields if f.startswith("ITEM"))
print(len(text), total, caps)
print(fields[n // 2], fields[-1])
