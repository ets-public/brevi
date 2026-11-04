# The typed build cannot take get() (a caught KeyError); the dynamic build takes the file and has CPython run
# shlex, which it lacks: the path line names it ("py").
import shlex


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


key = shlex.split("a b")[0]
b = Box({"a": 1})
print(b.get(key), b.get("z", 7))
