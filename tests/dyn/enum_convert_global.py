# globals() stores by a key known only at run time, Enum._convert_ and enum.global_enum (CPython 3.13.13 output)
import enum
MODULE = __name__
CONVERT_TEST_NAME_D = 5
CONVERT_TEST_NAME_C = 5
CONVERT_TEST_NAME_B = 5
CONVERT_TEST_NAME_A = 5
CONVERT_TEST_NAME_E = 6
UNCOMPARABLE_A = 5
UNCOMPARABLE_C = 'value'
UNCOMPARABLE_B = (9, 1)
COMPLEX_C = 1j
COMPLEX_A = 2j
COMPLEX_B = 3j
CONVERT_STR_TEST_2 = 'goodbye'
CONVERT_STR_TEST_1 = 'hello'
FLT_B = 2.5
FLT_A = 1

def reset():
    g = globals()
    for suffix in ['A', 'B', 'C', 'D']:
        g['CONVERT_TEST_NAME_%s' % suffix] = 5
        g['NEVER_READ_%s' % suffix] = suffix * 2
    g['CONVERT_TEST_NAME_E'] = 6
    print(g['NEVER_READ_B'], g.get('NEVER_READ_C'), g.get('NEVER_READ_X', 'dflt'), 'NEVER_READ_D' in g, 'NEVER_READ_X' not in g)

reset()
print(globals()['NEVER_READ_A'], 'NEVER_READ_A' in globals())
T = enum.IntEnum._convert_('UnittestConvert', MODULE, filter=lambda x: x.startswith('CONVERT_TEST_'))
print(repr(T.CONVERT_TEST_NAME_C), repr(T(5).name), repr(T(6)), list(T), T.__doc__, T.__module__, T.__qualname__)
print(repr(CONVERT_TEST_NAME_B), CONVERT_TEST_NAME_B is T.CONVERT_TEST_NAME_A, globals()['UnittestConvert'] is T)
print(sorted(T.__members__))
reset()
print(repr(CONVERT_TEST_NAME_B))
U = enum.Enum._convert_('Uncomparable', MODULE, filter=lambda x: x.startswith('UNCOMPARABLE_'))
print(list(U), [m.value for m in U])
C = enum.Enum._convert_('Cplx', MODULE, filter=lambda x: x.startswith('COMPLEX_'))
print(list(C))
S = enum.StrEnum._convert_('StrConvert', MODULE, lambda x: x.startswith('CONVERT_STR_') and not x.endswith('9'))
print(list(S), repr(S.CONVERT_STR_TEST_1), str(S.CONVERT_STR_TEST_2), format(S.CONVERT_STR_TEST_1))
F = enum.Enum._convert_('Flt', MODULE, filter=lambda n: n[:4] == 'FLT_', as_global=True)
print(list(F), repr(F.FLT_A), str(F.FLT_B), globals()['Flt'] is F)

@enum.global_enum
class Color(enum.Enum):
    RED = 1
    GREEN = 2
    CRIMSON = 1
print(repr(Color.RED), str(Color.GREEN), repr(Color.CRIMSON), globals()['CRIMSON'] is Color.RED)

@enum.global_enum
class Perm(enum.Flag):
    R = 4
    W = 2
    X = 1
print(repr(Perm.R), repr(Perm.R | Perm.W), repr(Perm(0)), str(Perm.R | Perm.X), str(Perm(0)))

@enum.global_enum
class Lights(enum.IntFlag, boundary=enum.KEEP):
    OFF = 0
    LOW = enum.auto()
    HIGH = enum.auto()
print(repr(Lights(0)), repr(Lights(1 + 4 + 16)), repr(Lights(8)), str(Lights.LOW), str(Lights(3)))

class Mood(enum.IntEnum):
    SAD = 1
    HAPPY = 2
enum.global_enum(Mood, update_str=True)
print(repr(Mood.SAD), str(Mood.HAPPY), format(Mood.HAPPY), globals()['HAPPY'] is Mood.HAPPY)
