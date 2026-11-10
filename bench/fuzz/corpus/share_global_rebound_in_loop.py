stack = []
def setk(k):
    if stack:
        stack[-1].append(k)
for i in range(4):
    if i == 1:
        stack.append([])
    setk(i)
    if i == 2:
        print(stack)
        stack = []
    setk(i)
print(stack)
