from enum import Enum, auto, unique

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3
    CRIMSON = 1

@unique
class Dir(Enum):
    N = auto()
    E = auto()
    S = auto()
    W = auto()

    def turn(self):
        order = list(Dir)
        return order[(order.index(self) + 1) % len(order)]

    @property
    def is_vertical(self):
        return self in (Dir.N, Dir.S)

class Planet(Enum):
    MERCURY = (3.303e+23, 2.4397e6)
    EARTH = (5.976e+24, 6.37814e6)

    def __init__(self, mass, radius):
        self.mass = mass
        self.radius = radius

    def gravity(self):
        return 6.673e-11 * self.mass / (self.radius * self.radius)

class Status(Enum):
    OPEN = "open"
    DONE = "done"

    def __str__(self):
        return self.value

print(Color.RED, repr(Color.GREEN), Color.CRIMSON is Color.RED, Color(2), Color["BLUE"])
print(list(Color), len(Color), Color.RED in Color, Color.RED.name, Color.RED.value)
print([d.name for d in Dir], Dir.N.value, Dir.W.turn(), Dir.E.is_vertical, Dir.S.is_vertical)
print(f"{Planet.EARTH.gravity():.2f}", Planet.MERCURY.mass, Planet((5.976e+24, 6.37814e6)))
print(Status.OPEN, str(Status.DONE), repr(Status.DONE), f"{Status.OPEN}", f"[{Color.RED}]")
print(Color, type(Color.RED).__name__, isinstance(Color.RED, Enum), isinstance(Color.RED, Color))
counts = {c: 0 for c in Color}
counts[Color.GREEN] += 2
print(counts, sorted(Color, key=lambda c: -c.value))
print(Color.RED == Color.RED, Color.RED == 1, Color.RED != Color.BLUE, hash(Color.RED) == hash(Color.RED))
print(list(Color.__members__), Color.__members__["CRIMSON"])
try:
    Color(7)
except ValueError as e:
    print("ValueError:", e)
try:
    Color["PINK"]
except KeyError as e:
    print("KeyError:", e)
try:
    @unique
    class Bad(Enum):
        A = 1
        B = 1
except ValueError as e:
    print("ValueError:", e)
try:
    Color.RED < Color.BLUE
except TypeError as e:
    print("TypeError:", e)
