class Box:
    def __init__(self, items):
        self.items = items
    def top(self):
        self.items.sort()
        return self.items[-1]
raw = [2, 9, 4]
b = Box(raw)
print(b.top(), raw)
