# Enum members through pickle (every protocol, memo shared), float.hex and float.fromhex (correctly rounded,
# subnormals, overflow, inf/nan, parse errors), and int.from_bytes / float.fromhex on subclasses (cls(value))
import pickle
from enum import Enum, IntEnum, Flag
class Color(Enum):
    RED = 1
    GREEN = 2
class N(IntEnum):
    A = 1
class P(Flag):
    R = 1
    W = 2
for proto in range(pickle.HIGHEST_PROTOCOL + 1):
    print(pickle.loads(pickle.dumps(Color.RED, protocol=proto)) is Color.RED,
          pickle.loads(pickle.dumps(N.A, protocol=proto)) is N.A,
          pickle.loads(pickle.dumps(P.R | P.W, protocol=proto)) == P.R | P.W,
          pickle.loads(pickle.dumps([Color.RED, Color.RED, N.A], protocol=proto)))
print(N.from_bytes(b'\x01', 'big'), type(N.from_bytes(b'\x01', 'big')))
class I(int): pass
print(type(I.from_bytes(b'\x01', 'big')))
class G(float): pass
print(type(G.fromhex('0x1p0')), G.fromhex('0x1p0'))
class F(float, Enum):
    X = 1.5
print(F.X.hex(), F.fromhex('0x1.8p0'))
tests = ['0x1.8p0', '-0x1p-1074', '0x1p-1075', '0x3p-1076', '0x1.fffffffffffff8p1023', '0x1.fffffffffffff7ffp1023', ' 0X1.ffffp10 ', 'inf', '-Infinity', 'nan', '1', '0x.8', '0x1.', '0', '-0x0p0', '0x1p99999999999999999999', '0x0p99999999999999', '0x1p-99999999999999999', '1.0000000000000011102230246251565404236316680908203125', 'x', '0x', '0x1p', '0xp1', '0x1p+', 'infin', '0x1.0000000000000800000000000001p0', '0x1.00000000000008p0', '0x1.00000000000018p0', '0x0.0000000000001p-1022', '0x1.0000000000001p-1023', '0x0.00000000000008p-1022', '0x0.00000000000018p-1022', '0x1p-1074', '0x1.8p-1074', '0x1.8p-1075']
for t in tests:
    try:
        v = float.fromhex(t)
        print(repr(t), repr(v), v.hex())
    except Exception as e:
        print(repr(t), type(e).__name__, e)
for x in [0.0, -0.0, 1.0, 5e-324, 2.2250738585072014e-308, 1.7976931348623157e308, -3.14, float('inf'), float('nan'), 1e-310]:
    print(x.hex())
