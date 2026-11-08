# A package of the program's that uses relative imports (`from .engine import Value`, `from . import nn`,
# `from ..engine import Value` in a subpackage) builds with the program.
import relgrad
from relgrad import Value, Neuron
from relgrad.nn import layer
from relgrad.nn.layer import kind

n = Neuron([0.5, -1.0, 2.0])
print(n([1, 2, 3]))
print(relgrad.nn.Layer([[1, 1], [2, -2]])([3, 4]))
print(layer.Layer([[1]])([7]), kind(), Value(2) * 3)
