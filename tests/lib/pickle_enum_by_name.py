# An Enum member pickled by name (its __reduce_ex__ gives a name, as enum.pickle_by_global_name does) is written as CPython's save_global writes it, memo
# included; one by value is (cls, (value,)); a by-name member that is not a module global cannot be pickled.
import enum, pickle
class G(enum.Enum):
    A = 1
    B = 2

    def __reduce_ex__(self, proto):
        return self.name
A = G.A
class H(enum.Enum):
    X = 1
for p in range(6):
    print(p, pickle.dumps([G.A, G.A, H.X, H], p))
print(pickle.loads(pickle.dumps(G.A)) is G.A, pickle.loads(pickle.dumps(H.X)) is H.X)
try:
    pickle.dumps(G.B)
except Exception as e:
    print(type(e).__name__, e)
