import json
from enum import Enum, auto
class M(str, Enum):
    A = 'a'
print(M.A, repr(M.A), f'{M.A}', M.A == 'a', format(M.A,'>5'), M.A.upper(), {1})
class N(int, Enum):
    A = 1
    B = 2
print(N.A, repr(N.A), f'{N.A}', N.A + 1, format(N.A, '>5'), '%s' % N.A, '%d' % N.B, N.A < N.B, sorted([N.B, N.A]), max(N), hash(N.A) == hash(1), {N.A: 3}[1])
from enum import Enum, IntEnum, StrEnum, auto
class S(StrEnum):
    A = auto()
class K(IntEnum):
    ONE = 1
class M(str, Enum):
    X = "x"
d = {"s": S.A, "k": K.ONE, "m": M.X, S.A: 2}
print(json.dumps(d), json.dumps([K.ONE, S.A]), "a".startswith(S.A), S.A in "abc", "abc".replace(S.A, "z"), S.A.split(), f"{K.ONE:03d}", round(K.ONE), K.ONE / 2, [0, 5][K.ONE], "xyz"[K.ONE], {1: "a"}[K.ONE], range(K.ONE + 2), str(S.A) + "!", S.A.value, len(M.X), sorted([M.X, "a"]))
