from ..engine import Value
from .. import engine


class Neuron:
    def __init__(self, ws):
        self.w = [Value(x) for x in ws]

    def __call__(self, xs):
        out = Value(0)
        for w, x in zip(self.w, xs):
            out = out + w * x
        return out


class Layer:
    def __init__(self, rows):
        self.neurons = [Neuron(r) for r in rows]

    def __call__(self, xs):
        return [n(xs) for n in self.neurons]


def kind():
    return engine.Value.__name__
