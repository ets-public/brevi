from collections import deque
from collections.abc import MutableSequence, Sequence, Reversible
print(isinstance(deque(), MutableSequence), issubclass(deque, MutableSequence), issubclass(deque, Sequence), isinstance(deque(), Reversible))
