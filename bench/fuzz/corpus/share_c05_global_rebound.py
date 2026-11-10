items = []
def add(v):
    items.append(v)
add(1)
print(items)
items = ["reset"]
add(3)
print(items)
count = 0
def bump():
    global count
    count += 1
bump()
count = 10.5
bump()
print(count)
