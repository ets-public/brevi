class Stack:
    def __init__(self):
        self.items = []

    def push(self, x):
        self.items.append(x)

    def first(self):
        return self.items[0]

    def is_empty(self):
        return len(self.items) == 0


def fill(s):
    s.push(1)
    s.push(2)


s = Stack()
fill(s)
print(s.first(), s.is_empty(), s.items)
