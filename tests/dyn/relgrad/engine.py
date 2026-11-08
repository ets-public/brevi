class Value:
    def __init__(self, data):
        self.data = data

    def __add__(self, other):
        return Value(self.data + (other.data if isinstance(other, Value) else other))

    def __mul__(self, other):
        return Value(self.data * (other.data if isinstance(other, Value) else other))

    def __repr__(self):
        return f"Value({self.data})"
