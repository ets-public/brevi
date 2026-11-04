class Stack:
    def __init__(self):
        self.items = []

    def push(self, x):
        self.items.append(x)


s = Stack()
s.push(1)
xs = [s.items[0]]
xs.push(2)
print(xs)
