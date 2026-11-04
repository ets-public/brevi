# The typed build cannot take get() (a caught KeyError), and the dynamic build cannot take eval():
# check names both, so fixing the first alone is not mistaken for enough.
import json


class Box:
    def __init__(self, data):
        self.data = data

    def __getitem__(self, key):
        return self.data[key]

    def get(self, key, default=None):
        try:
            return self[key]
        except KeyError:
            return default


key = eval(json.dumps("a b")).split()[0]
b = Box({"a": 1})
print(b.get(key), b.get("z", 7))
