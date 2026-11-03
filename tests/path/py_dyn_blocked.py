# The typed build cannot take get() (a caught KeyError), and the dynamic build cannot take argparse:
# check names both, so fixing the first alone is not mistaken for enough.
import argparse


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


ap = argparse.ArgumentParser()
ap.add_argument("--key", default="a")
args = ap.parse_args([])
b = Box({"a": 1})
print(b.get(args.key), b.get("z", 7))
